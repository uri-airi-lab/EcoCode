'use client'

import React, { memo, useState } from 'react'
import Link from 'next/link'
import { ProjectSummary } from '@/lib/api-client'
import { format } from 'date-fns'
import { ChevronRight, Folder } from 'lucide-react'
import {
  Pagination,
  PaginationContent,
  PaginationItem,
  PaginationLink,
  PaginationNext,
  PaginationPrevious,
} from '@/components/ui/pagination'

interface ProjectsTableProps {
    projects: ProjectSummary[]
}

const ITEMS_PER_PAGE = 5

export const ProjectsTable = memo(function ProjectsTable({ projects }: ProjectsTableProps) {
    const [currentPage, setCurrentPage] = useState(1)
    
    const totalPages = Math.ceil(projects.length / ITEMS_PER_PAGE)
    const startIndex = (currentPage - 1) * ITEMS_PER_PAGE
    const endIndex = startIndex + ITEMS_PER_PAGE
    const currentProjects = projects.slice(startIndex, endIndex)

    return (
        <div className="chart-container">
            <div className="flex items-center justify-between mb-4">
                <h3 className="text-lg font-semibold text-foreground flex items-center gap-2">
                    <Folder className="w-5 h-5 text-primary" />
                    Projects
                </h3>
                <Link
                    href="/projects"
                    className="text-sm text-primary hover:text-primary/80 transition-colors"
                >
                    View all →
                </Link>
            </div>

            <div className="overflow-x-auto">
                <table className="w-full text-sm">
                    <thead>
                        <tr className="border-b border-border">
                            <th className="text-left py-3 px-4 text-muted-foreground font-medium">
                                Project Name
                            </th>
                            <th className="text-center py-3 px-4 text-muted-foreground font-medium">
                                Total Runs
                            </th>
                            <th className="text-right py-3 px-4 text-muted-foreground font-medium">
                                Total Energy (Wh)
                            </th>
                            <th className="text-right py-3 px-4 text-muted-foreground font-medium">
                                Last Run
                            </th>
                            <th className="text-right py-3 px-4" />
                        </tr>
                    </thead>
                    <tbody>
                        {projects.length === 0 ? (
                            <tr>
                                <td colSpan={5} className="py-8 text-center text-muted-foreground">
                                    No projects found.
                                </td>
                            </tr>
                        ) : currentProjects.map((project) => (
                            <tr
                                key={project.id}
                                className="border-b border-border hover:bg-muted/30 transition-colors"
                            >
                                <td className="py-3 px-4">
                                    <p className="font-medium text-foreground">{project.name}</p>
                                </td>
                                <td className="text-center py-3 px-4 text-foreground font-medium">
                                    {project.totalRuns}
                                </td>
                                <td className="text-right py-3 px-4 text-foreground font-medium">
                                    {Number(project.totalEnergy.toFixed(2)).toLocaleString()}
                                </td>
                                <td className="text-right py-3 px-4 text-muted-foreground">
                                    {project.lastRunTimestamp ? format(new Date(project.lastRunTimestamp), 'MMM d, yyyy') : '-'}
                                </td>
                                <td className="text-right py-3 px-4">
                                    <Link href={`/projects/${project.id}`}>
                                        <ChevronRight className="w-4 h-4 text-muted-foreground hover:text-primary transition-colors" />
                                    </Link>
                                </td>
                            </tr>
                        ))}
                    </tbody>
                </table>
            </div>

            {totalPages > 1 && (
                <div className="mt-4 flex items-center justify-between border-t border-border pt-4">
                    <p className="text-xs text-muted-foreground">
                        Showing {startIndex + 1} to {Math.min(endIndex, projects.length)} of {projects.length} projects
                    </p>
                    <Pagination>
                        <PaginationContent>
                            <PaginationItem>
                                <PaginationPrevious 
                                    href="#" 
                                    onClick={(e) => {
                                        e.preventDefault()
                                        if (currentPage > 1) setCurrentPage(currentPage - 1)
                                    }}
                                    className={currentPage === 1 ? 'pointer-events-none opacity-50' : 'cursor-pointer'}
                                />
                            </PaginationItem>
                            
                            {Array.from({ length: totalPages }, (_, i) => (
                                <PaginationItem key={i}>
                                    <PaginationLink 
                                        href="#"
                                        isActive={currentPage === i + 1}
                                        onClick={(e) => {
                                            e.preventDefault()
                                            setCurrentPage(i + 1)
                                        }}
                                        className="cursor-pointer"
                                    >
                                        {i + 1}
                                    </PaginationLink>
                                </PaginationItem>
                            ))}

                            <PaginationItem>
                                <PaginationNext 
                                    href="#" 
                                    onClick={(e) => {
                                        e.preventDefault()
                                        if (currentPage < totalPages) setCurrentPage(currentPage + 1)
                                    }}
                                    className={currentPage === totalPages ? 'pointer-events-none opacity-50' : 'cursor-pointer'}
                                />
                            </PaginationItem>
                        </PaginationContent>
                    </Pagination>
                </div>
            )}
        </div>
    )
})
